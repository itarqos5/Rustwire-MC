// Original synthetic-fixture harness. Invokes installed release APIs only.
// Compile without dependencies; run with a prepared Paper jar and its libraries.
// Does not launch a server, load worlds, or open any network connection.
import java.io.ByteArrayOutputStream;
import java.lang.reflect.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
public final class RegistryTagsOracle {
    static Class<?> find(String... names) throws Exception {
        for (String n : names) try { return Class.forName(n); } catch (ClassNotFoundException ignored) {}
        throw new ClassNotFoundException(Arrays.toString(names));
    }
    static Method method(Class<?> owner, Class<?> result, Class<?>... args) {
        for (Method m : owner.getDeclaredMethods())
            if (m.getReturnType()==result && Arrays.equals(m.getParameterTypes(),args)) { m.setAccessible(true); return m; }
        throw new IllegalStateException(owner + " missing " + result + Arrays.toString(args));
    }
    static void integer(ByteArrayOutputStream b,int n) { do { int v=n & 127; n >>>= 7; b.write(v | (n==0?0:128)); } while(n!=0); }
    static void string(ByteArrayOutputStream b,String s) { byte[] bytes=s.getBytes(StandardCharsets.UTF_8); integer(b,bytes.length); b.writeBytes(bytes); }
    static byte[] tags(String registry,String name,boolean duplicates) {
        ByteArrayOutputStream b=new ByteArrayOutputStream(); integer(b,1); string(b,registry); integer(b,duplicates?2:1);
        if(duplicates) { string(b,name);integer(b,1);integer(b,3); }
        string(b,name);integer(b,4);for(int id:new int[]{0,128,2147483647,128}) integer(b,id);return b.toByteArray();
    }
    static Class<?> buffer,bytebuf;
    static Object make(byte[] bytes) throws Exception {
        Object raw=Class.forName("io.netty.buffer.Unpooled").getMethod("wrappedBuffer",byte[].class).invoke(null,(Object)bytes);
        return buffer.getConstructor(bytebuf).newInstance(raw);
    }
    static Object decode(Class<?> packet,byte[] bytes) throws Exception {
        Constructor<?> constructor=packet.getDeclaredConstructor(buffer); constructor.setAccessible(true); Object b=make(bytes);
        Object result=constructor.newInstance(b);
        if((int)bytebuf.getMethod("readableBytes").invoke(b)!=0)throw new AssertionError("remaining bytes");
        return result;
    }
    static byte[] encode(Class<?> packet,Object value) throws Exception {
        Object raw=Class.forName("io.netty.buffer.Unpooled").getMethod("buffer").invoke(null);
        Object b=buffer.getConstructor(bytebuf).newInstance(raw); method(packet,void.class,buffer).invoke(value,b);
        byte[] result=new byte[(int)bytebuf.getMethod("readableBytes").invoke(b)]; bytebuf.getMethod("readBytes",byte[].class).invoke(b,(Object)result); return result;
    }
    static void tagCase(Class<?> packet,String name,String registry,String tag,boolean duplicate) throws Exception {
        byte[] bytes=tags(registry,tag,duplicate);byte[] actual=encode(packet,decode(packet,bytes));
        String nr=registry.contains(":")?(registry.startsWith(":")?"minecraft"+registry:registry):"minecraft:"+registry;
        String nt=tag.contains(":")?(tag.startsWith(":")?"minecraft"+tag:tag):"minecraft:"+tag;
        if(!Arrays.equals(tags(nr,nt,false),actual))throw new AssertionError(name+" "+HexFormat.of().formatHex(actual));
        System.out.println(name+"="+HexFormat.of().formatHex(actual));
    }
    public static void main(String[] args) throws Exception {
        try {
            Class<?> constants=Class.forName("net.minecraft.SharedConstants");
            try { constants.getMethod("tryDetectVersion").invoke(null); }
            catch(NoSuchMethodException e) { constants.getMethod("a").invoke(null); }
            Class.forName("net.minecraft.server.DispenserRegistry").getMethod("a").invoke(null);
        } catch (ClassNotFoundException ignored) {}
        buffer=find("net.minecraft.network.FriendlyByteBuf","net.minecraft.network.PacketDataSerializer");bytebuf=Class.forName("io.netty.buffer.ByteBuf");
        Class<?> packet=find("net.minecraft.network.protocol.common.ClientboundUpdateTagsPacket","net.minecraft.network.protocol.game.PacketPlayOutTags");
        tagCase(packet,"canonical","test:registry","test:tag",false);
        tagCase(packet,"duplicates","test:registry","test:tag",true);
        byte[] one=tags("test:registry","test:tag",false);
        ByteArrayOutputStream repeated=new ByteArrayOutputStream();integer(repeated,2);repeated.writeBytes(Arrays.copyOfRange(one,1,one.length));repeated.writeBytes(Arrays.copyOfRange(one,1,one.length));
        if(!Arrays.equals(one,encode(packet,decode(packet,repeated.toByteArray()))))throw new AssertionError("duplicate registry");
        System.out.println("duplicate_registry=last_wins");
        tagCase(packet,"default_namespace","registry",":tag",false);
        tagCase(packet,"empty_paths","",":",false);
        for(String identifier:List.of("Bad:name","a:b:c","a/namespace:x","..:x")) {
            boolean accepted;try { decode(packet,tags(identifier,"tag",false)); accepted=true; }catch(InvocationTargetException e){accepted=false;}
            System.out.println("identifier["+identifier+"]="+accepted);
        }
        // From 766 the stream codec accepts any non-End NBT tag; not only compounds.
        try {
            Class<?> registry=Class.forName("net.minecraft.network.protocol.configuration.ClientboundRegistryDataPacket");
            Field codec;
            try { codec=registry.getField("STREAM_CODEC"); }
            catch(NoSuchFieldException e) {
                byte[] input=new byte[]{10,0};
                if(!Arrays.equals(input,encode(registry,decode(registry,input))))throw new AssertionError("legacy registry fixture");
                System.out.println("registry=0a00");return;
            }
            Object c=codec.get(null);
            Class<?> sc=Class.forName("net.minecraft.network.codec.StreamCodec");
            ByteArrayOutputStream b=new ByteArrayOutputStream();string(b,"test:registry");integer(b,2);string(b,"test:omitted");b.write(0);string(b,"test:present");b.writeBytes(new byte[]{1,3,0,0,0,7});
            byte[] input=b.toByteArray();Object value=sc.getMethod("decode",Object.class).invoke(c,make(input));
            Object out=buffer.getConstructor(bytebuf).newInstance(Class.forName("io.netty.buffer.Unpooled").getMethod("buffer").invoke(null));
            sc.getMethod("encode",Object.class,Object.class).invoke(c,out,value);byte[] actual=new byte[(int)bytebuf.getMethod("readableBytes").invoke(out)];bytebuf.getMethod("readBytes",byte[].class).invoke(out,(Object)actual);
            if(!Arrays.equals(input,actual))throw new AssertionError("registry fixture");System.out.println("registry="+HexFormat.of().formatHex(actual));
        } catch (ClassNotFoundException | NoSuchFieldException e) { System.out.println("registry=legacy"); }
    }
}
