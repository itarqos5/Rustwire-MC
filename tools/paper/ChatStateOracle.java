// Original test harness invoking installed official chat-state APIs. No game implementation.
// Compile without dependencies; run with a prepared Paper jar and its libraries on the classpath.
import java.lang.reflect.*;
import java.time.Instant;
import java.util.*;
public final class ChatStateOracle {
    static Class<?> cls(String name) throws Exception { return Class.forName("net.minecraft.network.chat." + name); }
    static Method method(Class<?> owner, Class<?> result, Class<?>... args) {
        for (Method m : owner.getDeclaredMethods())
            if (Modifier.isPublic(m.getModifiers()) && m.getReturnType()==result && Arrays.equals(m.getParameterTypes(), args)) return m;
        throw new IllegalStateException(owner + " missing " + result + Arrays.toString(args));
    }
    static Object component(Object record, int index) throws Exception { RecordComponent c=record.getClass().getRecordComponents()[index]; Method m=c.getAccessor(); return (m==null?method(record.getClass(),c.getType()):m).invoke(record); }
    static Object signature(int n) throws Exception { byte[] b=new byte[256]; b[255]=(byte)n; return cls("MessageSignature").getConstructor(byte[].class).newInstance((Object)b); }
    static int id(Object signature) throws Exception { return ((byte[])method(signature.getClass(),byte[].class).invoke(signature))[255] & 255; }
    static void emit(Object tracker, String name) throws Exception {
        Object generated=method(tracker.getClass(), Arrays.stream(tracker.getClass().getDeclaredClasses()).filter(Class::isRecord).findFirst().orElseThrow()).invoke(tracker);
        Object seen=component(generated,0), update=component(generated,1);
        List<Integer> ids=new ArrayList<>(); for(Object s:(List<?>)component(seen,0))ids.add(id(s));
        RecordComponent[] fields=update.getClass().getRecordComponents();
        int check=fields.length==3?((Byte)component(update,2)) & 255:-1;
        System.out.println(name+" offset="+component(update,0)+" bits="+Arrays.toString(((BitSet)component(update,1)).toByteArray())+" checksum="+check+" seen="+ids);
    }
    public static void main(String[] args) throws Exception {
        Class<?> s=cls("MessageSignature"), t=cls("LastSeenMessagesTracker"), c=cls("MessageSignatureCache");
        Method add=method(t,boolean.class,s,boolean.class), ignore=method(t,void.class,s);
        Object tracker=t.getConstructor(int.class).newInstance(20);
        emit(tracker,"empty");
        add.invoke(tracker,signature(1),true); System.out.println("duplicate="+add.invoke(tracker,signature(1),true)); emit(tracker,"one");
        add.invoke(tracker,signature(2),false); add.invoke(tracker,signature(3),true); ignore.invoke(tracker,signature(3)); emit(tracker,"ignored");
        ignore.invoke(tracker,signature(1)); emit(tracker,"cannot_remove_acknowledged");
        for(int i=4;i<=25;i++)add.invoke(tracker,signature(i),true); emit(tracker,"wrapped"); emit(tracker,"repeat");
        Object zero=t.getConstructor(int.class).newInstance(20); add.invoke(zero,signature(224),true); emit(zero,"zero_checksum");
        Object cache=c.getConstructor(int.class).newInstance(128);
        Class<?> seen=cls("LastSeenMessages"), body=cls("SignedMessageBody");
        Method push; boolean legacy;
        try { push=method(c,void.class,body,s); legacy=false; }
        catch(IllegalStateException e) { push=method(c,void.class,cls("PlayerChatMessage")); legacy=true; }
        Method pack=method(c,int.class,s);
        Constructor<?> makeBody=body.getConstructor(String.class,Instant.class,long.class,seen);
        Object a=signature(1), b=signature(2), d=signature(3);
        for(List<Object> input:List.of(List.of(a,b),List.of(b,a,b),List.of(d))) {
            Object history=seen.getConstructor(List.class).newInstance(input);
            Object messageBody=makeBody.newInstance("fixture",Instant.EPOCH,0L,history), messageSignature=input.size()==1?null:d;
            if(legacy) {
                Object link=cls("SignedMessageLink").getConstructor(int.class,UUID.class,UUID.class).newInstance(0,new UUID(0,0),new UUID(0,1));
                Object filter=cls("FilterMask").getConstructor(int.class).newInstance(0);
                Constructor<?> makeMessage=Arrays.stream(cls("PlayerChatMessage").getConstructors()).filter(k->k.getParameterCount()==5).findFirst().orElseThrow();
                push.invoke(cache,makeMessage.newInstance(link,messageSignature,messageBody,null,filter));
            } else push.invoke(cache,messageBody,messageSignature);
            List<Integer> positions=new ArrayList<>(); for(int i=1;i<=4;i++)positions.add((Integer)pack.invoke(cache,signature(i)));
            System.out.println("cache="+positions);
        }
    }
}
