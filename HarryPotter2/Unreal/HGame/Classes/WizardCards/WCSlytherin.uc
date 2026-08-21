//===============================================================================
//  #48 Salazar Slytherin
//===============================================================================

class  WCSlytherin extends Goldcards;

#EXEC TEXTURE IMPORT NAME=WizardCardSlytherinTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Slytherinsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardSlytherinBigTexture FILE=TEXTURES\menu\Folio\Cards\Slytherinbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

#EXEC TEXTURE IMPORT NAME=WizCardSalazarBigTextureLayer0 FILE=TEXTURES\menu\Folio\Cards\48_Salazar_1.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardSalazarBigTextureLayer1 FILE=TEXTURES\menu\Folio\Cards\48_Salazar_2.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=WizCardSalazarBigTextureLayer2 FILE=TEXTURES\menu\Folio\Cards\48_Salazar_3.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Salazar Slytherin";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=48
    skin(0)=Texture'HProps.skins.WizardCardSlytherinTex0'
	textureBig=Texture'WizCardSlytherinBigTexture'
	strDescriptionId="WizCard_0086"

	bIsLayered=true;
	textureLayers(0)=Texture'WizCardSalazarBigTextureLayer0';
	textureLayers(1)=Texture'WizCardSalazarBigTextureLayer1';
	textureLayers(2)=Texture'HPParticle.salazarfire';
	bLastLayerIsFire=true;

}
