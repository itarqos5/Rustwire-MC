// Original fixture harness: capture bytes passed to the official signature updater.
// Does not obtain credentials, generate private keys, sign, or connect to a server.
import java.lang.reflect.*;
import java.time.Instant;
import java.util.*;
import java.io.ByteArrayOutputStream;
public final class ChatSigningOracle {
    static final java.io.PrintStream OUTPUT=System.out;
    static Class<?> chat(String n) throws Exception { return Class.forName("net.minecraft.network.chat."+n); }
    static void fixture(String label,String content,long millis,List<Object> previous) throws Exception {
        Class<?> updater=Class.forName("net.minecraft.util.SignatureUpdater");
        Class<?> output=Arrays.stream(updater.getDeclaredClasses()).filter(Class::isInterface).findFirst().orElseThrow();
        ByteArrayOutputStream bytes=new ByteArrayOutputStream();
        Object receiver=Proxy.newProxyInstance(output.getClassLoader(),new Class[]{output},(proxy,method,args)->{ bytes.write((byte[])args[0]);return null; });
        Object link=chat("SignedMessageLink").getConstructor(int.class,UUID.class,UUID.class).newInstance(7,UUID.fromString("00112233-4455-6677-8899-aabbccddeeff"),UUID.fromString("ffeeddcc-bbaa-9988-7766-554433221100"));
        Object seen=chat("LastSeenMessages").getConstructor(List.class).newInstance(previous);
        Object body=chat("SignedMessageBody").getConstructor(String.class,Instant.class,long.class,chat("LastSeenMessages")).newInstance(content,Instant.ofEpochMilli(millis),-17L,seen);
        Method target=Arrays.stream(chat("PlayerChatMessage").getDeclaredMethods()).filter(m->Modifier.isPublic(m.getModifiers())&&Modifier.isStatic(m.getModifiers())&&Arrays.equals(m.getParameterTypes(),new Class[]{output,chatUnchecked("SignedMessageLink"),chatUnchecked("SignedMessageBody")})).findFirst().orElseThrow();
        target.invoke(null,receiver,link,body);
        OUTPUT.println(label+"="+HexFormat.of().formatHex(bytes.toByteArray()));
    }
    static Class<?> chatUnchecked(String name) { try { return chat(name); } catch(Exception e) { throw new IllegalStateException(e); } }
    public static void main(String[] args) throws Exception {
        Class<?> constants=Class.forName("net.minecraft.SharedConstants");
        try { constants.getMethod("tryDetectVersion").invoke(null); }
        catch(NoSuchMethodException e) { constants.getMethod("a").invoke(null); }
        try { Class.forName("net.minecraft.server.Bootstrap").getMethod("bootStrap").invoke(null); }
        catch(ClassNotFoundException e) { Class.forName("net.minecraft.server.DispenserRegistry").getMethod("a").invoke(null); }
        fixture("empty","",0L,List.of());
        byte[] signature=new byte[256];Arrays.fill(signature,(byte)0xa5);
        Object s=chat("MessageSignature").getConstructor(byte[].class).newInstance((Object)signature);
        fixture("unicode_negative_millis","caf\u00e9 \ud83d\ude00",-1L,List.of(s));
        fixture("positive_millis","test",1999L,List.of());
    }
}
